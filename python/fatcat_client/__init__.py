# coding: utf-8

# flake8: noqa

"""
    fatcat

    A scalable, versioned, API-oriented catalog of bibliographic entities and file metadata  # noqa: E501

    OpenAPI spec version: 0.1.0
    
    Generated by: https://github.com/swagger-api/swagger-codegen.git
"""


from __future__ import absolute_import

# import apis into sdk package
from fatcat_client.api.default_api import DefaultApi

# import ApiClient
from fatcat_client.api_client import ApiClient
from fatcat_client.configuration import Configuration
# import models into sdk package
from fatcat_client.models.changelogentries import Changelogentries
from fatcat_client.models.changelogentries_inner import ChangelogentriesInner
from fatcat_client.models.container_entity import ContainerEntity
from fatcat_client.models.creator_entity import CreatorEntity
from fatcat_client.models.editgroup import Editgroup
from fatcat_client.models.editgroup_edits import EditgroupEdits
from fatcat_client.models.editor import Editor
from fatcat_client.models.entity_edit import EntityEdit
from fatcat_client.models.error_response import ErrorResponse
from fatcat_client.models.file_entity import FileEntity
from fatcat_client.models.release_contrib import ReleaseContrib
from fatcat_client.models.release_entity import ReleaseEntity
from fatcat_client.models.release_ref import ReleaseRef
from fatcat_client.models.success import Success
from fatcat_client.models.work_entity import WorkEntity
